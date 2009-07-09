class EventsController < ApplicationController
  before_filter :load_calendar
  
  def bookmarklet
    render :action => "bookmarklet", :layout => false
  end
  
  # GET /events
  # GET /events.xml
  def index
    @events = Event.find(:all)

    respond_to do |format|
      format.html # index.html.erb
      format.xml  { render :xml => @events }
    end
  end

  # GET /events/1
  # GET /events/1.xml
  def show
    @event = Event.find(params[:id])

    respond_to do |format|
      format.html # show.html.erb
      format.xml  { render :xml => @event }
    end
  end

  # GET /events/new
  # GET /events/new.xml
  def new
    @event = @calendar.events.build

    respond_to do |format|
      format.html # new.html.erb
      format.xml  { render :xml => @event }
    end
  end
  
  # GET /events/newbookmarklet
  
  def newbookmark
    @bookmarklet = bookmarklet_params(params)
    @event = @calendar.events.build
        
    respond_to do |format|
      format.html # newbookmark.html.erb
      format.xml  { render :xml => @event }
    end
  end
  
  # GET /events/1/edit
  def edit
    @event = Event.find(params[:id])
  end

  # POST /events
  # POST /events.xml
  def create
    @event = @calendar.events.build(params[:event])

    respond_to do |format|
      if @event.save
        flash[:notice] = 'Event was successfully created.'
        format.html { redirect_to([@calendar,@event]) }
        format.xml  { render :xml => @event, :status => :created, :location => @event }
      else
        format.html { render :action => "new" }
        format.xml  { render :xml => @event.errors, :status => :unprocessable_entity }
      end
    end
  end

  # PUT /events/1
  # PUT /events/1.xml
  def update
    @event = Event.find(params[:id])

    respond_to do |format|
      if @event.update_attributes(params[:event])
        flash[:notice] = 'Event was successfully updated.'
        format.html { redirect_to([@calendar,@event]) }
        format.xml  { head :ok }
      else
        format.html { render :action => "edit" }
        format.xml  { render :xml => @event.errors, :status => :unprocessable_entity }
      end
    end
  end

  # DELETE /events/1
  # DELETE /events/1.xml
  def destroy
    @event = Event.find(params[:id])
    @event.destroy

    respond_to do |format|
      format.html { redirect_to(@calendar) }
      format.xml  { head :ok }
    end
  end
  
  protected
  
  def bookmarklet_params(params)
    bm = {}
    bm[:version]    = params[:v]
    bm[:string]     = params[:s]
    bm[:page_title] = params[:t]
    bm[:page_url]   = params[:u]
    return bm
  end
  
  def load_calendar
    @calendar = Calendar.find(params[:calendar][:id]) if params[:calendar] && params[:calendar][:id]
    @calendar = Calendar.find(params[:calendar_id]) if params[:calendar_id]
    return redirect_to(home_path) if @calendar.nil?
  end
  
end
