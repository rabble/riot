require File.expand_path(File.dirname(__FILE__) + "/../test_helper")

class EventsControllerTest < ActionController::TestCase
  before :all do
    @calendar = Calendar.make 
  end
  
  test "should create a new event class with submited bookmarklet info" do
    pending "got to fix this" do
      bookmarklet_hash = {"v"=>"3", "s"=>"Friday, January 23, 2009\nMoth Mainstage\nIn Harm's Way: Stories about Danger\n\nat The Players\n\n16 Gramercy Park South\n\n(between Irivng Place and Park Avenue South)\n\n6:30pm Doors open\n\n7:30pm Stories start on stage\n\nCurated by Meg Bowles\n\nHosted by comedian Tom Shillue\n\nStories by New York Times best-selling writer Amy Cohen (The Late Bloomer’s Revolution), Sudanese refuge and “Lost Boy” John Dau (God Grew Tired of Us), former undercover FBI agent, and New York Times best-selling... (read more) ", "t"=>"The Moth - Upcoming Events", "u"=>"http://www.themoth.org/events/"}
      get :new, bookmarklet_hash, :calendar => @calendar
      assigns(:bookmarklet)[:version].should eql(bookmarklet_hash['v'])
      assigns(:bookmarklet)[:string].should eql(bookmarklet_hash['s'])
      assigns(:bookmarklet)[:page_title].should eql(bookmarklet_hash['t'])
      assigns(:bookmarklet)[:page_url].should eql(bookmarklet_hash['u'])
    end
  end

  test "should get index" do
    get :index, :calendar_id => @calendar.id
    assert_response :success
    assert_not_nil assigns(:events)
  end

  test "should get new" do
    get :new, :calendar_id => @calendar.id
    assert_response :success
  end

  test "should create event" do
    
    assert_difference('Event.count') do
      post :create, {:event => { }, :calendar_id => @calendar.id}
    end
    
    assert_redirected_to [@calendar, assigns(:event)]
    assert_equal assigns(:event).calendar, @calendar
  end

  test "should show event" do
    get :show, :id => Event.make.id, :calendar_id => @calendar.id
    assert_response :success
  end

  test "should get edit" do
    get :edit, :id => Event.make.id, :calendar_id => @calendar.id
    assert_response :success
  end

  test "should update event" do
    put :update, :id => Event.make.id, :event => { }, :calendar_id => @calendar.id
    assert_redirected_to calendar_event_path(@calendar, assigns(:event))
  end

  test "should destroy event" do
    event = Event.make
    assert_difference('Event.count', -1) do
      delete :destroy, :id => event.id, :calendar_id => @calendar.id
    end

    assert_redirected_to @calendar
  end
  
  
  def test_should_route_events_in_a_calendar
    options = {:controller => 'events', :action => 'show', :calendar_id => "2", :id => '1'}
    #at some point it'd be nicer to have /calendar/2/event/1... but i'm not sure about the routes
    assert_routing('calendars/2/events/1', options)
  end
  
  describe_requests do
    setup = lambda do
      #stub(@controller).current_user { admin_user }
    end
    
    context "should get new with calendar" do

      act! { get :new, {:calendar => {:id => @calendar.id}} }

      it_renders :template, :new
      it_assigns :calendar
      it_assigns :event
    end

  end
end
