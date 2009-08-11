class CalendarsController < ApplicationController
  require 'json/pure' 
  before_filter :load_current_time
  ActiveRecord::Base.include_root_in_json = false
  
  # GET /calendars
  # GET /calendars.xml
  def index
    @calendars = Calendar.find(:all)
    
    
    respond_to do |format|
      format.html # index.html.erb
      format.xml  { render :xml => @calendars }
    end
  end

  # GET /calendars/1
  # GET /calendars/1.xml
  def show
    @calendar = Calendar.find(params[:id])

    respond_to do |format|
      format.html # show.html.erb
      format.xml  { render :xml => @calendar }
      format.json { render :json => @calendar.events_in_json(:start => Time.at(params[:start].to_i), :end => Time.at(params[:end].to_i)) }
    end
  end
  
  def feed
    @calendar = Calendar.find(params[:id])
    
    #at some point we need to distinguish between 
    respond_to do |format|
      format.xml  { render :xml => @calendar }
      format.json { render :json => @calendar.events_in_json(:start => params[:start], :end => params[:end]) }
    end
  end
  
  def day
    @calendar = Calendar.find(params[:id] || params[:calendar_id])
    @events = @calendar.events_on_day(@date)

    respond_to do |format|
      format.html # show.html.erb
      format.xml  { render :xml => @calendar }
    end
  end

  # GET /calendars/new
  # GET /calendars/new.xml
  def new
    @calendar = Calendar.new

    respond_to do |format|
      format.html # new.html.erb
      format.xml  { render :xml => @calendar }
    end
  end
  
  # GET /calendars/newbookmarklet
  
  def newbookmark
    @bookmarklet = bookmarklet_params(params)
    @calendar = Calendar.new
    
    respond_to do |format|
      format.html # newbookmark.html.erb
      format.xml  { render :xml => @calendar }
    end
  end
  
  # GET /calendars/1/edit
  def edit
    @calendar = Calendar.find(params[:id])
  end

  # POST /calendars
  # POST /calendars.xml
  def create
    @calendar = Calendar.new(params[:calendar])

    respond_to do |format|
      if @calendar.save
        flash[:notice] = 'Calendar was successfully created.'
        format.html { redirect_to(@calendar) }
        format.xml  { render :xml => @calendar, :status => :created, :location => @calendar }
      else
        format.html { render :action => "new" }
        format.xml  { render :xml => @calendar.errors, :status => :unprocessable_entity }
      end
    end
  end

  # PUT /calendars/1
  # PUT /calendars/1.xml
  def update
    @calendar = Calendar.find(params[:id])

    respond_to do |format|
      if @calendar.update_attributes(params[:calendar])
        flash[:notice] = 'Calendar was successfully updated.'
        format.html { redirect_to(@calendar) }
        format.xml  { head :ok }
      else
        format.html { render :action => "edit" }
        format.xml  { render :xml => @calendar.errors, :status => :unprocessable_entity }
      end
    end
  end

  # DELETE /calendars/1
  # DELETE /calendars/1.xml
  def destroy
    @calendar = Calendar.find(params[:id])
    @calendar.destroy

    respond_to do |format|
      format.html { redirect_to(calendars_url) }
      format.xml  { head :ok }
    end
  end
  
  private
  
  def load_current_time
    if params[:year] && params[:month]
    
      @year = params[:year].to_i || Time.now.year
      @month = params[:month].to_i || Time.now.month
      @day = params[:day].to_i || Time.now.day
      @date = Date.civil(@year, @month, @day)
    else
      @year = Time.now.year
      @month = Time.now.month
      @day = Time.now.day
      
      @date = Time.now.to_date
    end
  end
end
