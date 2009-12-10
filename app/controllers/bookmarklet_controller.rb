class BookmarkletController < ApplicationController
  skip_before_filter :verify_authenticity_token, :except => :newbookmark
  
  def bookmarklet
    respond_to do |format|
      format.js # bookmarklet.js.erb
    end
  end
  
  def parse
    @event = Event.parse(params)
    @calendar = Calendar.find(:first)
    
    respond_to do |format|
      format.html { render 'events/new' }
      format.xml  { render :xml => @event }
    end
  end
  
  # GET /events/newbookmarklet
  #not used anymore
  def newbookmark
    @bookmarklet = bookmarklet_params(params)
    @event = @calendar.events.build
        
    respond_to do |format|
      format.html # newbookmark.html.erb
      format.xml  { render :xml => @event }
    end
  end
  
  

end
